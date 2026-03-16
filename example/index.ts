import { Tachyon } from 'tachyon-rs'

new Tachyon({ compressionThreshold: -1 })
  .get('/', 'Hello Tachyon!')
  .get('/mingau', {
    response: 'mingau'
  })
  .get('/grande', {
    response: Array.from({ length: 100 }).map((_, i) => ({
      id: i,
      name: 'User ' + i,
      data: 'a'.repeat(500)
    }))
  })
  .listen(3000)
